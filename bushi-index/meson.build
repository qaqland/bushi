project(
    'bushi-index',
    'c',
    meson_version: '>= 1.3.0',
    version: '0.1',
    default_options: ['warning_level=3', 'c_std=c23'],
)

main_deps = [
    dependency('sqlite3'),
    dependency('libgit2'),
    # MIT
]

test_deps = [
    dependency('sqlite3'),
    dependency('libgit2'),
    dependency('cunit'),  # GPL
]

exe = executable(
    'bushi-index',
    'bushi-index.c',
    dependencies: main_deps,
    install: true,
)

test = executable(
    'bushi-index-test',
    'test.c',
    dependencies: test_deps,
    install: false,
)

test('basic', test)
