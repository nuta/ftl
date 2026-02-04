#!/bin/bash
#
#  Usage: ./deploy-to-google-cloud.sh <image-bucket-name>
#
if [ -z "$1" ]; then
    echo "Usage: ./deploy-to-google-cloud.sh <image-bucket-name>"
    exit 1
fi

set -xue

IMAGE_NAME=ftl
BUCKET_PATH="gs://$1/ftl.tar.gz"

# brew install i686-elf-grub
GRUB_MKRESCUE=/opt/homebrew/opt/i686-elf-grub/bin/i686-elf-grub-mkrescue

BUILD_ONLY=1 ./run.sh

mkdir -p isofiles/boot/grub
cp kernel/src/arch/x64/grub.cfg isofiles/boot/grub/
cp ftl.elf initfs.tar isofiles/

$GRUB_MKRESCUE -o ftl.iso isofiles

if [[ -n ${ISO_ONLY:-} ]]; then
    exit 0
fi

# Create a raw disk image:
# https://docs.cloud.google.com/compute/docs/import/import-existing-image#create_image_file
cp ftl.iso disk.raw

# oldgnu format is required for Google Cloud:
# https://discuss.google.dev/t/gcp-custom-vm-created-from-virtualbox/155646/3
gtar --format=oldgnu -Sczf google-cloud-image.tar.gz disk.raw
rm disk.raw

gcloud storage cp google-cloud-image.tar.gz "$BUCKET_PATH"
gcloud compute images create "$IMAGE_NAME" --source-uri "$BUCKET_PATH"

echo
echo "FTL OS image deployed to Google Cloud and ready to create a VM"
