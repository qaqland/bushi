project(
    'bushi-index',
    'c',
    meson_version: '>= 1.3.0',
    version: '0.1',
    default_options: ['warning_level=3', 'c_std=c23'],
    license: 'MIT',
)

deps = [
    dependency('sqlite3'),
    dependency('libgit2'),
]

exe = executable(
    'bushi-index',
    'bushi-index.c',
    dependencies: deps,
    install: true,
)

