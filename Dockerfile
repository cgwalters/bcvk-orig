FROM scratch as dependencies
COPY *dependencies.txt /

FROM quay.io/centos/centos:stream10 as base
WORKDIR /src
RUN --mount=type=bind,from=dependencies,target=/run/deps <<EORUN
set -xeuo pipefail
dnf config-manager --set-enabled crb
dnf -y install https://dl.fedoraproject.org/pub/epel/epel-release-latest-10.noarch.rpm
grep -vE -e '^#' /run/deps/dependencies.txt | xargs dnf -y install
dnf clean all
rm -rf /var/{cache,tmp,log}/*
EORUN

FROM base as buildroot
RUN --mount=type=bind,from=dependencies,target=/run/deps <<EORUN
set -xeuo pipefail
# Build dependencies
grep -vE -e '^#' /run/deps/build-dependencies.txt | xargs dnf -y install
EORUN
# Only now copy the full source code so source changes don't blow out the package caches
COPY . /src
# See https://www.reddit.com/r/rust/comments/126xeyx/exploring_the_problem_of_faster_cargo_docker/
# We aren't using the full recommendations there, just the simple bits.
RUN --mount=type=cache,target=/src/target \ 
    --mount=type=cache,target=/root \
    make && make install DESTDIR=/out

FROM base
COPY --from=buildroot /out/ /
ENTRYPOINT ["bootc-kit"]

