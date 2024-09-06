FROM scratch

COPY target/x86_64-unknown-linux-musl/release/commandeer /commandeer

ENTRYPOINT [ "/commandeer" ]
