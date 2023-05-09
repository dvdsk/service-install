#!/usr/bin/env sh

# using fedora as it has dnf --installroot which is needed to install
# packages in the container without root
ctr=$(buildah from fedora) 
# mnt=$(buildah mount $ctr)

# buildah run $ctr dnf -y install systemd
buildah run $ctr /bin/sh -c 'dnf -y install systemd'
buildah config \
	--author="renewc" \
	--entrypoint '["/usr/sbin/init"]' \
	--workingdir='/root' $ctr

buildah commit $ctr test_img
