#!/bin/sh
case "$1" in
metadata) cat metadata.json ;;
tree) case "$*" in
*wasm32*) printf "krilla v0.8.2 (pinned)|raster-images,simple-text\n" ;;
*) printf "krilla v0.8.2 (pinned)|raster-images,rayon,simple-text\n" ;;
esac ;;
*) exit 91 ;;
esac
