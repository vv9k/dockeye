# dockeye

[![Build Status](https://github.com/vv9k/dockeye/workflows/dockeye%20CI/badge.svg)](https://github.com/vv9k/dockeye/actions?query=workflow%3A%22dockeye+CI%22)

> GUI app for managing Docker

# Instalation

## Prebuilt packages
Download one of the archives available on the [releases page](https://github.com/vv9k/dockeye/releases) and extract it somewhere in your `$PATH`

## AUR

To install from AUR, use your favorite AUR helper like `paru`:
```shell
$ paru -S dockeye
```

## Building from source

Install required libraries (only required on Linux):

### APT
```shell
$ apt-get install libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev
```

### Pacman
```Shell
$ pacman -S libxcb libxkbcommon
```

To install **dockeye** you'll need the latest rust with cargo. To build run:
```shell
$ cargo build --release
```
and later copy `./target/release/dockeye` to your `$PATH`.


![usage](https://github.com/vv9k/dockeye/blob/master/usage.webp)

## License
[GPLv3](https://github.com/vv9k/dockeye/blob/master/LICENSE)
