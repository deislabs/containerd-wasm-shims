## FAQ
If you ever see this build error, please do the following:
```
   Compiling spiderlightning v0.1.0 (https://github.com/deislabs/spiderlightning?rev=e66843889ded777806475ba580c4f1fe86ec53a3#e6684388)
   Compiling rdkafka-sys v4.2.0+1.8.2
   Compiling wasmtime-runtime v0.39.1
   Compiling cranelift-wasm v0.86.1
   Compiling oci-spec v0.5.8
error: failed to run custom build command for `rdkafka-sys v4.2.0+1.8.2`
  ...
  
  --- stderr
  Building and linking librdkafka statically
  CMake Error at /home/linuxbrew/.linuxbrew/Cellar/cmake/3.24.1/share/cmake/Modules/FindPackageHandleStandardArgs.cmake:230 (message):
    Could NOT find ZLIB (missing: ZLIB_LIBRARY ZLIB_INCLUDE_DIR)
  ...
```

Run:
- `sudo apt-get install zlib1g-dev`