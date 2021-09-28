FROM alpine:latest
ARG TARGETARCH
ARG TARGETVARIANT
RUN apk --no-cache add ca-certificates tini
RUN apk add tzdata && \
	cp /usr/share/zoneinfo/Asia/Shanghai /etc/localtime && \
	echo "Asia/Shanghai" > /etc/timezone && \
	apk del tzdata

WORKDIR /data/
ADD douban-api-rs-$TARGETARCH$TARGETVARIANT /usr/bin/douban-api-rs

ENTRYPOINT ["/sbin/tini", "--"]
CMD ["/usr/bin/douban-api-rs", "--port", "80"]