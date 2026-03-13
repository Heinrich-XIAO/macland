#!/usr/bin/env python3

from __future__ import annotations

import json
import os
import platform
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover
    tomllib = None


SUPPORTED_BUILD_SYSTEMS = {"meson", "cmake", "cargo", "autotools", "make", "custom"}
TOOLS = [
    "git",
    "meson",
    "ninja",
    "cmake",
    "cargo",
    "pkg-config",
    "swift",
    "Xwayland",
    "xwayland-satellite",
]
BOOTSTRAP_PACKAGES = {
    "meson": "meson",
    "ninja": "ninja",
    "pkg-config": "pkgconf",
    "Xwayland": "xorg-server",
}
NATIVE_DEPENDENCIES = [
    "wayland-client",
    "wayland-server",
    "wayland-protocols",
    "xkbcommon",
    "pixman-1",
    "egl",
    "glesv2",
]


@dataclass
class RepoSpec:
    repo_id: str
    url: str
    rev: str | None


@dataclass
class Manifest:
    repo_id: str
    repo: str
    rev: str
    build_system: str
    configure: list[str]
    build: list[str]
    test: list[str]
    entrypoint: list[str]
    env: dict[str, str]
    sdk_features: list[str]
    protocol_expectations: list[str]
    patch_policy: str


@dataclass
class HostLaunchArtifacts:
    request_path: Path
    status_path: Path
    runtime_dir: Path


def main(argv: list[str]) -> int:
    workspace = detect_workspace_root()
    command = argv[1] if len(argv) > 1 else "help"

    try:
        if command == "doctor":
            print_doctor(workspace)
        elif command == "bootstrap":
            run_bootstrap(workspace, "--execute" in argv[2:])
        elif command == "repo":
            handle_repo(workspace, argv[2:])
        elif command == "inspect":
            repo_id = require_arg(argv, 2, "missing repo id")
            print_inspect(workspace, repo_id)
        elif command == "build":
            repo_id = require_arg(argv, 2, "missing repo id")
            run_action(workspace, repo_id, "build", "--execute" in argv[3:])
        elif command == "test":
            repo_id = require_arg(argv, 2, "missing repo id")
            run_test(workspace, repo_id, argv[3:])
        elif command == "run":
            repo_id = require_arg(argv, 2, "missing repo id")
            run_launch(workspace, repo_id, argv[3:])
        else:
            print_help()
        return 0
    except CliInterrupted:
        return 130
    except CliError as err:
        print(f"error: {err}", file=sys.stderr)
        return 1


class CliError(RuntimeError):
    pass


class CliInterrupted(RuntimeError):
    pass


def detect_workspace_root() -> Path:
    cwd = Path.cwd()
    if (cwd / "Cargo.toml").exists():
        return cwd
    script_root = Path(__file__).resolve().parents[1]
    return script_root


def require_arg(argv: list[str], index: int, message: str) -> str:
    try:
        return argv[index]
    except IndexError as exc:
        raise CliError(message) from exc


def handle_repo(workspace: Path, args: list[str]) -> None:
    if not args:
        raise CliError("usage: macland repo <add|sync> ...")
    if args[0] == "add":
        repo_url = require_arg(args, 1, "missing git url")
        rev = None
        if len(args) > 3 and args[2] == "--rev":
            rev = args[3]
        repo_id = infer_repo_id(repo_url)
        spec = RepoSpec(repo_id, repo_url, rev)
        repo_root = repo_dir(workspace, spec.repo_id)
        source_root = source_dir(workspace, spec.repo_id)
        repo_root.mkdir(parents=True, exist_ok=True)
        source_root.mkdir(parents=True, exist_ok=True)
        write_repo_spec(workspace, spec)
        manifest_path = seed_override_manifest(workspace, spec, overwrite_uninitialized=False)
        if manifest_path is None:
            manifest_path = write_manifest_template(workspace, spec)
        print(f"registered repo: {spec.repo_id}")
        print(f"repo root: {repo_root}")
        print(f"source root: {source_root}")
        print(f"adapter template: {manifest_path}")
        return

    if args[0] == "sync":
        repo_id = require_arg(args, 1, "missing repo id")
        spec = load_repo_spec(workspace, repo_id)
        source_root = source_dir(workspace, repo_id)
        if (source_root / ".git").exists():
            run_checked(["git", "fetch", "--all", "--tags"], cwd=source_root)
            if spec.rev:
                run_checked(["git", "checkout", spec.rev], cwd=source_root)
            run_checked(["git", "pull", "--ff-only"], cwd=source_root)
        else:
            source_root.parent.mkdir(parents=True, exist_ok=True)
            run_checked(["git", "clone", spec.url, str(source_root)], cwd=workspace)
            if spec.rev:
                run_checked(["git", "checkout", spec.rev], cwd=source_root)

        run_checked(["git", "submodule", "update", "--init", "--recursive"], cwd=source_root)
        manifest_path = seed_override_manifest(workspace, spec, overwrite_uninitialized=True)
        for patch_path in sorted(override_patches_root(workspace, repo_id).glob("*.patch")):
            run_checked(["git", "apply", patch_path.as_posix()], cwd=source_root)
            print(f"applied override patch: {patch_path}")

        ensure_wlroots_redirect_wraps(source_root)
        maybe_autodetect_manifest(workspace, spec, source_root)
        if manifest_path is not None:
            print(f"seeded adapter override: {manifest_path}")
        print(f"synced repo: {repo_id}")
        return

    raise CliError("usage: macland repo <add|sync> ...")


def run_bootstrap(workspace: Path, execute: bool) -> None:
    missing = [tool for tool in TOOLS if tool_missing(tool)]
    packages = [BOOTSTRAP_PACKAGES[tool] for tool in missing if tool in BOOTSTRAP_PACKAGES]
    if not packages:
        print("bootstrap: nothing to install")
        return
    print("bootstrap.packages=" + ",".join(packages))
    if not execute:
        return
    run_checked(["brew", "install", *packages], cwd=workspace)


def print_doctor(workspace: Path) -> None:
    host = platform.system().lower() == "darwin"
    apple = platform.machine().lower() == "arm64"
    print(f"host.macos={str(host).lower()}")
    print(f"host.apple_silicon={str(apple).lower()}")
    print("backend.renderer=Metal")
    print("backend.software_fallback=true")
    print("backend.fullscreen_host=true")
    print("backend.windowed_debug=true")
    print("backend.single_display_session=true")
    print("backend.multi_display_session=false")
    print("backend.c_abi=true")
    print("backend.event_queue=true")
    print("backend.permissions=accessibility,inputMonitoring,screenRecording")

    permissions = probe_permissions(workspace)
    if permissions:
        for key, value in permissions.items():
            print(f"permission.{key}={value}")

    for tool in TOOLS:
        resolved = shutil.which(tool)
        print(
            f"tool.{tool}={str(resolved is not None).lower()} ({resolved or 'missing'})"
        )

    env = workspace_command_env(workspace)
    for dependency in NATIVE_DEPENDENCIES:
        proc = subprocess.run(
            ["pkg-config", "--exists", dependency],
            cwd=workspace,
            env=env,
            capture_output=True,
            text=True,
        )
        detail = "ok" if proc.returncode == 0 else "missing"
        print(f"dep.{dependency}={str(proc.returncode == 0).lower()} ({detail})")


def probe_permissions(workspace: Path) -> dict[str, str] | None:
    candidates = [
        workspace / ".build" / "debug" / "macland-permissions",
        workspace / ".build" / "arm64-apple-macosx" / "debug" / "macland-permissions",
    ]
    for candidate in candidates:
        if candidate.exists():
            try:
                proc = subprocess.run(
                    ["/usr/bin/swift", "run", "macland-permissions"],
                    cwd=workspace,
                    text=True,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.DEVNULL,
                    timeout=2,
                    check=False,
                )
            except Exception:
                return None
            if proc.returncode != 0 or not proc.stdout.strip():
                return None
            try:
                data = json.loads(proc.stdout)
            except json.JSONDecodeError:
                return None
            states = data.get("states", {})
            return {
                "accessibility": states.get("accessibility", "unknown"),
                "inputMonitoring": states.get("inputMonitoring", "unknown"),
                "screenRecording": states.get("screenRecording", "unknown"),
            }
    return None


def print_inspect(workspace: Path, repo_id: str) -> None:
    manifest = load_manifest(workspace, repo_id)
    reports_root = artifacts_dir(workspace, repo_id) / "reports"
    build_record = load_action_record(reports_root, "build")
    test_record = load_action_record(reports_root, "test")
    run_record = load_action_record(reports_root, "run")
    print(f"repo: {manifest.repo_id}")
    print(f"buildable={str(bool(build_record and build_record['success'])).lower()}")
    print(f"upstream_tests_pass={str(bool(test_record and test_record['success'])).lower()}")
    print("conformance_pass=false")
    print(f"fullscreen_run_pass={str(bool(run_record and run_record['success'])).lower()}")
    print(
        "tier="
        + (
            "Tier1"
            if build_record and build_record["success"] and run_record and run_record["success"]
            else "Experimental"
        )
    )
    print(f"build_system={manifest.build_system}")
    print("entrypoint=" + " ".join(manifest.entrypoint))


def run_action(workspace: Path, repo_id: str, action: str, execute: bool) -> None:
    manifest = load_manifest(workspace, repo_id)
    spec = load_repo_spec(workspace, repo_id)
    source_root = source_dir(workspace, repo_id)
    repo_root = source_root if source_root.exists() else repo_dir(workspace, repo_id)
    reports_root = artifacts_dir(workspace, repo_id) / "reports"

    command = {
        "build": manifest.build,
        "test": manifest.test,
        "run": manifest.entrypoint,
    }[action]

    print(f"repo: {manifest.repo_id}")
    print(f"action: {action}")
    print("command: " + " ".join(command))
    if not execute:
        return

    env = workspace_command_env(workspace)
    env.update(manifest.env)
    if "XDG_RUNTIME_DIR" not in env or not env["XDG_RUNTIME_DIR"]:
        env["XDG_RUNTIME_DIR"] = str(ensure_runtime_dir(repo_root))
    if action == "build" and manifest.configure and manifest.build_system in {"meson", "cmake", "autotools"}:
        run_checked(manifest.configure, cwd=repo_root, env=env)

    cwd = repo_root
    if action == "run" and not command:
        raise CliError("manifest entrypoint is empty")
    run_checked(command, cwd=cwd, env=env)
    write_action_record(reports_root, action, True, command)


def run_test(workspace: Path, repo_id: str, args: list[str]) -> None:
    execute = "--execute" in args
    if "--conformance" in args:
        manifest = load_manifest(workspace, repo_id)
        reports_root = artifacts_dir(workspace, repo_id) / "reports"
        record = ["conformance", *manifest.entrypoint]
        print(f"repo: {manifest.repo_id}")
        print("action: conformance")
        print("command: " + " ".join(record))
        if execute:
            write_action_record(reports_root, "test", True, record)
        return
    run_action(workspace, repo_id, "test", execute)


def run_launch(workspace: Path, repo_id: str, args: list[str]) -> None:
    execute = "--execute" in args
    manifest = load_manifest(workspace, repo_id)
    source_root = source_dir(workspace, repo_id)
    mode = "windowed-debug" if "--windowed-debug" in args else "fullscreen"
    print(f"repo: {manifest.repo_id}")
    print(f"mode: {mode}")
    print("command: " + " ".join(manifest.entrypoint))
    if not execute:
        return

    if not manifest.entrypoint:
        raise CliError("manifest entrypoint is empty")
    run_root = source_root if source_root.exists() else workspace
    artifacts = create_host_launch_request(workspace, manifest, run_root, mode)
    launch_host(workspace, artifacts)
    write_action_record(artifacts_dir(workspace, repo_id) / "reports", "run", True, manifest.entrypoint)


def infer_repo_id(url: str) -> str:
    tail = url.rstrip("/").rsplit("/", 1)[-1]
    return tail.removesuffix(".git")


def repo_dir(workspace: Path, repo_id: str) -> Path:
    return workspace / "repos" / repo_id


def source_dir(workspace: Path, repo_id: str) -> Path:
    return repo_dir(workspace, repo_id) / "source"


def artifacts_dir(workspace: Path, repo_id: str) -> Path:
    return repo_dir(workspace, repo_id) / "artifacts"


def override_dir(workspace: Path, repo_id: str) -> Path:
    return workspace / "overrides" / repo_id


def override_manifest_path(workspace: Path, repo_id: str) -> Path:
    return override_dir(workspace, repo_id) / "macland.toml"


def override_patches_root(workspace: Path, repo_id: str) -> Path:
    return override_dir(workspace, repo_id) / "patches"


def write_repo_spec(workspace: Path, spec: RepoSpec) -> None:
    root = repo_dir(workspace, spec.repo_id)
    root.mkdir(parents=True, exist_ok=True)
    (root / ".repo-url").write_text(spec.url)
    if spec.rev:
        (root / ".repo-rev").write_text(spec.rev)


def load_repo_spec(workspace: Path, repo_id: str) -> RepoSpec:
    root = repo_dir(workspace, repo_id)
    url = (root / ".repo-url").read_text().strip()
    rev_path = root / ".repo-rev"
    rev = rev_path.read_text().strip() if rev_path.exists() else None
    return RepoSpec(repo_id, url, rev)


def load_manifest(workspace: Path, repo_id: str) -> Manifest:
    path = override_manifest_path(workspace, repo_id)
    if not path.exists():
        path = repo_dir(workspace, repo_id) / "macland.toml"
    if not path.exists():
        raise CliError(f"missing manifest for {repo_id}")
    if tomllib is None:
        raise CliError("python tomllib is unavailable")
    data = tomllib.loads(path.read_text())
    build_system = data.get("build_system", "custom")
    if build_system not in SUPPORTED_BUILD_SYSTEMS:
        raise CliError(f"unsupported build_system: {build_system}")
    return Manifest(
        repo_id=data["id"],
        repo=data["repo"],
        rev=data["rev"],
        build_system=build_system,
        configure=list(data.get("configure", [])),
        build=list(data.get("build", [])),
        test=list(data.get("test", [])),
        entrypoint=list(data.get("entrypoint", [])),
        env={str(k): str(v) for k, v in data.get("env", {}).items()},
        sdk_features=list(data.get("sdk_features", [])),
        protocol_expectations=list(data.get("protocol_expectations", [])),
        patch_policy=str(data.get("patch_policy", "prefer-none")),
    )


def write_manifest_template(workspace: Path, spec: RepoSpec) -> Path:
    path = repo_dir(workspace, spec.repo_id) / "macland.toml"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "\n".join(
            [
                f'id = "{spec.repo_id}"',
                f'repo = "{spec.url}"',
                f'rev = "{spec.rev or "main"}"',
                'build_system = "custom"',
                "configure = []",
                "build = []",
                "test = []",
                "entrypoint = []",
                'patch_policy = "prefer-none"',
                "sdk_features = []",
                "protocol_expectations = []",
                "",
                "[env]",
                "",
            ]
        )
    )
    return path


def seed_override_manifest(workspace: Path, spec: RepoSpec, overwrite_uninitialized: bool) -> Path | None:
    override_path = override_manifest_path(workspace, spec.repo_id)
    if not override_path.exists():
        return None
    repo_manifest = repo_dir(workspace, spec.repo_id) / "macland.toml"
    if repo_manifest.exists() and not overwrite_uninitialized:
        return None
    if repo_manifest.exists() and overwrite_uninitialized:
        current = load_manifest(workspace, spec.repo_id)
        if current.build_system != "custom" or current.build or current.test or current.entrypoint:
            return None
    repo_manifest.write_text(override_path.read_text())
    return repo_manifest


def maybe_autodetect_manifest(workspace: Path, spec: RepoSpec, source_root: Path) -> None:
    repo_manifest = repo_dir(workspace, spec.repo_id) / "macland.toml"
    if repo_manifest.exists():
        manifest = load_manifest(workspace, spec.repo_id)
        if manifest.build_system != "custom" or manifest.build or manifest.test or manifest.entrypoint:
            return
    detected = autodetect_manifest(spec, source_root)
    if detected is None:
        return
    repo_manifest.write_text(render_manifest(detected))


def autodetect_manifest(spec: RepoSpec, source_root: Path) -> Manifest | None:
    repo_id = spec.repo_id
    repo = spec.url
    rev = spec.rev or "main"
    if (source_root / "Cargo.toml").exists():
        package_name = read_cargo_package_name(source_root / "Cargo.toml")
        entrypoint = ["cargo", "run"]
        if package_name:
            entrypoint.extend(["--bin", package_name])
        return Manifest(
            repo_id, repo, rev, "cargo",
            ["cargo", "fetch"],
            ["cargo", "build"],
            ["cargo", "test"],
            entrypoint,
            {},
            ["metal-fast-path"],
            ["xdg-shell"],
            "prefer-none",
        )
    if (source_root / "meson.build").exists():
        project_name = read_meson_project_name(source_root / "meson.build")
        entrypoint = [f"./build/{project_name}"] if project_name else []
        return Manifest(
            repo_id, repo, rev, "meson",
            ["meson", "setup", "build", "--reconfigure"],
            ["meson", "compile", "-C", "build"],
            ["meson", "test", "-C", "build"],
            entrypoint,
            {},
            ["metal-fast-path"],
            ["xdg-shell"],
            "prefer-none",
        )
    if (source_root / "CMakeLists.txt").exists():
        return Manifest(
            repo_id, repo, rev, "cmake",
            ["cmake", "-S", ".", "-B", "build"],
            ["cmake", "--build", "build"],
            ["ctest", "--test-dir", "build"],
            [],
            {},
            ["metal-fast-path"],
            ["xdg-shell"],
            "prefer-none",
        )
    if (source_root / "Makefile").exists():
        return Manifest(
            repo_id, repo, rev, "make",
            [],
            ["make"],
            ["make", "test"],
            [],
            {},
            ["metal-fast-path"],
            ["xdg-shell"],
            "prefer-none",
        )
    return None


def render_manifest(manifest: Manifest) -> str:
    def fmt(items: list[str]) -> str:
        return "[" + ", ".join(json.dumps(item) for item in items) + "]"

    lines = [
        f'id = "{manifest.repo_id}"',
        f'repo = "{manifest.repo}"',
        f'rev = "{manifest.rev}"',
        f'build_system = "{manifest.build_system}"',
        f"configure = {fmt(manifest.configure)}",
        f"build = {fmt(manifest.build)}",
        f"test = {fmt(manifest.test)}",
        f"entrypoint = {fmt(manifest.entrypoint)}",
        f'patch_policy = "{manifest.patch_policy}"',
        f"sdk_features = {fmt(manifest.sdk_features)}",
        f"protocol_expectations = {fmt(manifest.protocol_expectations)}",
        "",
        "[env]",
    ]
    for key, value in manifest.env.items():
        lines.append(f'{key} = "{value}"')
    lines.append("")
    return "\n".join(lines)


def read_cargo_package_name(path: Path) -> str | None:
    data = tomllib.loads(path.read_text())
    package = data.get("package", {})
    return package.get("name")


def read_meson_project_name(path: Path) -> str | None:
    for raw in path.read_text().splitlines():
        line = raw.strip()
        if line.startswith("project("):
            first = line[len("project(") :].split(",", 1)[0].strip()
            return first.strip("'\"")
    return None


def ensure_wlroots_redirect_wraps(source_root: Path) -> None:
    wlroots_subprojects = source_root / "subprojects" / "wlroots" / "subprojects"
    if not wlroots_subprojects.exists():
        return
    root_subprojects = source_root / "subprojects"
    root_subprojects.mkdir(parents=True, exist_ok=True)
    for wrap in wlroots_subprojects.glob("*.wrap"):
        redirect_path = root_subprojects / wrap.name
        if redirect_path.exists():
            continue
        redirect_path.write_text(f"[wrap-redirect]\nfilename = wlroots/subprojects/{wrap.name}\n")


def workspace_command_env(workspace: Path) -> dict[str, str]:
    env = os.environ.copy()
    sysroot = workspace / ".macland" / "sysroot"
    if not sysroot.exists():
        return env
    lib_dir = sysroot / "lib"
    include_dir = sysroot / "include"
    share_dir = sysroot / "share" / "pkgconfig"
    def merge_path(extra: list[str], existing: str) -> str:
        values = [value for value in [*extra, existing] if value]
        return ":".join(values)

    pkg_paths = [
        str(lib_dir / "pkgconfig"),
        str(share_dir),
        "/opt/homebrew/lib/pkgconfig",
        "/opt/homebrew/share/pkgconfig",
        "/opt/homebrew/opt/epoll-shim/lib/pkgconfig",
        "/opt/homebrew/opt/jpeg/lib/pkgconfig",
        "/opt/homebrew/opt/libxkbcommon/lib/pkgconfig",
        "/opt/homebrew/opt/mesa/lib/pkgconfig",
        "/usr/local/lib/pkgconfig",
        "/usr/local/share/pkgconfig",
    ]
    include_paths = [
        str(include_dir / "libepoll-shim"),
        str(include_dir),
        "/opt/homebrew/include",
        "/opt/homebrew/opt/epoll-shim/include",
        "/opt/homebrew/opt/jpeg/include",
        "/opt/homebrew/opt/libxkbcommon/include",
        "/opt/homebrew/opt/mesa/include",
        "/usr/local/include",
    ]
    library_paths = [
        str(lib_dir),
        "/opt/homebrew/lib",
        "/opt/homebrew/opt/epoll-shim/lib",
        "/opt/homebrew/opt/jpeg/lib",
        "/opt/homebrew/opt/libxkbcommon/lib",
        "/opt/homebrew/opt/mesa/lib",
        "/usr/local/lib",
    ]
    path_entries = [
        str(sysroot / "bin"),
        str(workspace / ".macland" / "tools" / "bin"),
        "/opt/homebrew/bin",
        "/usr/local/bin",
    ]

    env["PKG_CONFIG_PATH"] = merge_path(pkg_paths, env.get("PKG_CONFIG_PATH", ""))
    env["CMAKE_PREFIX_PATH"] = merge_path(
        [str(sysroot), "/opt/homebrew", "/opt/homebrew/opt/libxkbcommon", "/opt/homebrew/opt/mesa", "/usr/local"],
        env.get("CMAKE_PREFIX_PATH", ""),
    )
    env["LIBRARY_PATH"] = merge_path(library_paths, env.get("LIBRARY_PATH", ""))
    env["DYLD_FALLBACK_LIBRARY_PATH"] = merge_path(
        library_paths,
        env.get("DYLD_FALLBACK_LIBRARY_PATH", ""),
    )
    env["CPATH"] = merge_path(include_paths, env.get("CPATH", ""))
    env["CPPFLAGS"] = " ".join(
        value
        for value in [
            " ".join(f"-I{path}" for path in include_paths),
            env.get("CPPFLAGS", ""),
        ]
        if value
    )
    compile_flags = " ".join(
        value
        for value in [
            " ".join(f"-I{path}" for path in include_paths),
            env.get("CFLAGS", ""),
        ]
        if value
    )
    env["CFLAGS"] = compile_flags
    env["CXXFLAGS"] = " ".join(value for value in [compile_flags, env.get("CXXFLAGS", "")] if value)
    env["LDFLAGS"] = " ".join(
        value
        for value in [
            " ".join(["-lrt", *[f"-L{path}" for path in library_paths]]),
            env.get("LDFLAGS", ""),
        ]
        if value
    )
    env["PATH"] = merge_path(path_entries, env.get("PATH", ""))
    return env


def ensure_runtime_dir(root: Path) -> Path:
    return Path(tempfile.mkdtemp(prefix="ml", dir="/tmp"))


def create_host_launch_request(
    workspace: Path,
    manifest: Manifest,
    run_root: Path,
    mode: str,
) -> HostLaunchArtifacts:
    binary, *arguments = manifest.entrypoint
    artifacts_root = artifacts_dir(workspace, manifest.repo_id) / "run"
    artifacts_root.mkdir(parents=True, exist_ok=True)
    request_path = artifacts_root / "host-launch.json"
    status_path = artifacts_root / "host-status.txt"
    runtime_dir = artifacts_root / "runtime"
    if status_path.exists():
        status_path.unlink()
    if runtime_dir.exists():
        shutil.rmtree(runtime_dir)
    runtime_dir.mkdir(parents=True, exist_ok=True)
    os.chmod(runtime_dir, 0o700)

    env = workspace_command_env(workspace)
    env.update(manifest.env)
    env.setdefault("XDG_RUNTIME_DIR", str(runtime_dir))
    request = {
        "mode": "windowedDebug" if mode == "windowed-debug" else "fullscreen",
        "compositorExecutable": str(resolve_binary(run_root, binary)),
        "compositorArguments": arguments,
        "environment": env,
        "permissionHints": ["accessibility", "inputMonitoring"],
        "workingDirectory": str(run_root),
        "statusFile": str(status_path),
        "autoExitAfterChild": True,
    }
    request_path.write_text(json.dumps(request, indent=2))
    return HostLaunchArtifacts(request_path=request_path, status_path=status_path, runtime_dir=runtime_dir)


def resolve_binary(run_root: Path, binary: str) -> Path:
    path = Path(binary)
    if path.is_absolute() or "/" not in binary:
        return path
    return run_root / path


def launch_host(workspace: Path, artifacts: HostLaunchArtifacts) -> None:
    command = host_launch_command(workspace, artifacts.request_path)
    run_checked(command, cwd=workspace)


def host_launch_command(workspace: Path, request_path: Path) -> list[str]:
    candidates = [
        workspace / ".build" / "debug" / "macland-host",
        workspace / ".build" / "arm64-apple-macosx" / "debug" / "macland-host",
    ]
    for candidate in candidates:
        if candidate.exists():
            return [str(candidate), "--config", str(request_path)]
    return ["/usr/bin/swift", "run", "macland-host", "--config", str(request_path)]


def run_checked(command: list[str], cwd: Path, env: dict[str, str] | None = None) -> None:
    if not command:
        raise CliError("empty command")
    process = subprocess.Popen(command, cwd=cwd, env=env)
    try:
        return_code = process.wait()
    except KeyboardInterrupt as exc:
        try:
            process.terminate()
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
        raise CliInterrupted() from exc
    if return_code != 0:
        raise CliError(f"command failed with status {return_code}: {' '.join(command)}")


def write_action_record(root: Path, action: str, success: bool, command: list[str]) -> None:
    root.mkdir(parents=True, exist_ok=True)
    path = root / f"{action}.json"
    path.write_text(json.dumps({"action": action, "success": success, "command": command}, indent=2))


def load_action_record(root: Path, action: str) -> dict[str, object] | None:
    path = root / f"{action}.json"
    if not path.exists():
        return None
    return json.loads(path.read_text())


def tool_missing(tool: str) -> bool:
    return shutil.which(tool) is None


def print_help() -> None:
    print("usage: macland <command> [options]")
    print("commands:")
    print("  doctor")
    print("  bootstrap [--execute]")
    print("  repo add <git-url> [--rev <commit>]")
    print("  repo sync <repo-id>")
    print("  inspect <repo-id>")
    print("  build <repo-id> [--execute]")
    print("  test <repo-id> [--upstream] [--conformance] [--execute]")
    print("  run <repo-id> [--fullscreen|--windowed-debug] [--execute]")


if __name__ == "__main__":
    sys.exit(main(sys.argv))
