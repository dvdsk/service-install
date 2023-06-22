#!/usr/bin/env bash

# currently broken issue reported: 
# https://github.com/containers/buildah/issues/2611

set -e

ctr=$(buildah from fedora) 
echo "ctr: $ctr"
# mnt=$(buildah mount $ctr)

buildah run "$ctr" /bin/sh -c 'dnf -y install systemd'
buildah config \
	--author="renewc" \
	--entrypoint '["/usr/sbin/init"]' \
	--workingdir='/root' "$ctr"

buildah commit "$ctr" "systemd"
