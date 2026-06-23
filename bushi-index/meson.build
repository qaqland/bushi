project(
    'bushi-index',
    'c',
    meson_version: '>= 1.3.0',
    version: '0.1',
    default_options: [
        'warning_level=3',
        'c_std=c23',
        'buildtype=release',
        'strip=true',
        'b_lto=true',
    ],
    license: 'GPL-2.0-only',
)

root = meson.current_source_dir()

fs = import('fs')
if not fs.exists('git/libgit.a')
    error('libgit.a not found. Run "make libgit" first.')
endif

libgit = declare_dependency(
    include_directories: include_directories('git'),
    dependencies: meson.get_compiler('c').find_library(
        'git',
        dirs: [root / 'git'],
        static: true,
    ),
)

deps = [
    dependency('sqlite3'),
    dependency('zlib'),
    libgit,
]

exe = executable(
    'bushi-index',
    'bushi-index.c',
    dependencies: deps,
    install: true,
)

