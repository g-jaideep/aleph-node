#!/bin/bash

set -eu

pushd local-tests/

if [ ! -f "$BINARY" ]; then
  echo "$BINARY does not exist."
  exit 1
fi

if [ ! -f "$FLOODER" ]; then
  echo "$FLOODER does not exist."
  exit 1
fi

echo 'Preparing environment'
chmod +x $FLOODER $BINARY

pip install -r requirements.txt


echo 'Running test'
./test_flood.py

popd
