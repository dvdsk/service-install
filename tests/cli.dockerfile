FROM fedora
USER root

RUN dnf -y install systemd
ENTRYPOINT ["/usr/sbin/init"]

#notes
# https://yast.opensuse.org/blog/2023-02-28/systemd-podman-github-ci
# https://developers.redhat.com/blog/2019/04/24/how-to-run-systemd-in-a-container#other_cool_features_about_podman_and_systemd
