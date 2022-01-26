FROM alpine:latest
ARG TARGETARCH
ARG TARGETVARIANT
RUN apk --no-cache add ca-certificates tini && \
apk add tzdata && \
cp /usr/share/zoneinfo/Asia/Shanghai /etc/localtime && \
echo "Asia/Shanghai" > /etc/timezone && \
apk del tzdata

WORKDIR /data/
ADD douban-api-rs-$TARGETARCH$TARGETVARIANT /usr/bin/douban-api-rs

# 生成启动脚本
RUN printf '#!/bin/sh \n\n\
\n\
/usr/bin/douban-api-rs --port 80  \n\
\n\
' > /entrypoint.sh && \
chmod +x /entrypoint.sh

ENTRYPOINT ["/sbin/tini", "--"]
CMD ["/entrypoint.sh"]
