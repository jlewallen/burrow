#!/bin/bash

# TODO Traps

if [ -z $HOST ]; then
	echo "usage: HOST=server deploy.sh"
	exit 2
fi

set -xe

docker save jlewallen/burrow -o /tmp/burrow.tar

rsync -vua --progress /tmp/burrow.tar $HOST:

ssh $HOST docker load -i burrow.tar

ssh $HOST docker stop burrow || true
ssh $HOST docker rm -f burrow || true
ssh $HOST docker run --name burrow -d --rm -p 5000:3000 -v /home/jlewallen/burrow:/app/data \
	-e RUST_LOG=debug,tower_http=debug \
	jlewallen/burrow \
	/app/cli serve --path /app/data/world.sqlite3

ssh $HOST docker image prune --all -f
