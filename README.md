# todo-mvc-bench
A benchmarking suite for TodoMVC implementations

## why
I want to write the fastest TodoMVC and the existing benchmarks are funky!
https://github.com/schell/mogwai/issues/18

## building
Building should be as easy as `bash ./scripts/build.sh`. This should build two
things:

  1. A `release` directory containing the built benchmark web app.
  2. A `release.tar.gz` of the `release` directory.

## running
After building use [basic-http-server](https://crates.io/crates/basic-http-server)
or your favorite server to host the `release` directory:

```
basic-http-server -a 127.0.0.1:8888 release
```

## Happy hacking!
:coffee: :coffee: :coffee:
