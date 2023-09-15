ARG CROSS_BASE_IMAGE
FROM $CROSS_BASE_IMAGE

RUN apt-get -y update
RUN apt-get install -y pkg-config libsystemd-dev libdbus-glib-1-dev libelf-dev libseccomp-dev

RUN --mount=type=bind,source=/,target=/polyfill /polyfill/polyfill.sh
