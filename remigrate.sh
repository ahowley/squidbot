#!/usr/bin/bash

function migrate() {
    num_migrations=$(($(ls 2>/dev/null -Ub1 -- ./migrations | wc -l) / 2))
    for i in {1..$num_migrations}
    do
        sqlx migrate revert
    done
    sqlx migrate run
}

migrate
cp .env .env.temp
cp .env.test .env
migrate
cp .env.temp .env
rm .env.temp