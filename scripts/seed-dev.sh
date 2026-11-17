#!/usr/bin/env sh
set -eu

exec cargo run -p cookbook-infra --bin cookbook-seed -- "$@"
