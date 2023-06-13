#!/usr/bin/env bash

set -e

buildah unshare tests/buildah.sh
# podman build --file tests/cli.dockerfile --force-rm . -t service-install-systemd-test
#
# # now we need to do a little dance because the image is owned by root
# # see: https://github.com/containers/podman/issues/5608
#
# dir=$(mktemp -d)
# podman save service-install-systemd-test --output "$dir/image.tar"
# podman load --input "$dir/image.tar"
