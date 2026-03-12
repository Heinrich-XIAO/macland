project('demo-meson-compositor', 'c')

demo = executable('demo-meson-compositor', 'src/main.c', install: false)
test('demo-meson-compositor-self-test', demo, args: ['--self-test'])
