#!/usr/bin/env bash

set -e

# sudo ./tests/buildah.sh
sudo podman build --file tests/cli.dockerfile --force-rm . -t service-install-systemd-test

# now we need to do a little dance because the image is owned by root
# see: https://github.com/containers/podman/issues/5608

dir=$(mktemp)
sudo podman save service-install-systemd-test -o $dir/image.tar
podman import $dir/image.tar service-install-systemd-test
