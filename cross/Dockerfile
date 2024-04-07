ARG CROSS_BASE_IMAGE
FROM $CROSS_BASE_IMAGE

# Install Alpine's `apk` to be able to install musl packages
COPY --from=jorgeprendes420/apk-anywhere / /
ENV MARCH=${CROSS_CMAKE_SYSTEM_PROCESSOR}
RUN apk-init ${MARCH} ${CROSS_SYSROOT}

# configure libsecccomp-rs to use static linking
RUN apk-${MARCH} add libseccomp-static libseccomp-dev
ENV LIBSECCOMP_LINK_TYPE="static"
ENV LIBSECCOMP_LIB_PATH="${CROSS_SYSROOT}/lib"

# wws needs zlib (though libssh2-sys)
RUN apk-${MARCH} add zlib-dev zlib-static

RUN apt-get -y update && \
    apt-get install -y pkg-config protobuf-compiler
