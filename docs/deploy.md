# Deploy

FTL supports several hosting platforms.

## microVM Cloud

[microVM Cloud](https://microvm.cloud) is the most convenient way to run FTL.

1. Build FTL with embedded initrd and boot args:

```
RELEASE=1 EMBED_SYSTEM_ELF=1 APPEND_CMDLINE="ftl.run=/bin/util ftl.arg=httpd" ./build.sh
```

2. [Open the images page](https://microvm.cloud/#/images) and upload `ftl.elf` as a new image.
3. [Open the new VM page](https://microvm.cloud/#/vms/new) and create a new VM with the uploaded image.

> [!TIP]
>
> You can also use the `microvmcloud` CLI to automate the process. See the [API & Keys](https://microvm.cloud/#/api) page for more details.

## Google Compute Engine

Google Cloud supports [custom operating systems](https://docs.cloud.google.com/compute/docs/images/building-custom-os) and offers a free tier, which is more than enough to run FTL.

1. Build FTL in ISO format (`ftl.iso`):

```
RELEASE=1 ISO=1 ./build.sh
```

2. Create a disk image (tarball) for GCP:

```
cp ftl.iso disk.raw
tar --format=oldgnu -Sczf ftl.tar.gz disk.raw
```

3. Upload the disk image to GCP and create an image from it:

```
gcloud storage cp ftl.tar.gz gs://<YOUR_BUCKET_NAME>/
gcloud compute images create ftl --source-uri=gs://<YOUR_BUCKET_NAME>/ftl.tar.gz
```

4. Create a VM instance:

```
gcloud compute instances create <INSTANCE_NAME> --image=ftl
```

> [!NOTE]
>
> You may also want to add a firewall rule to allow incoming HTTP traffic to the VM instance.

> [!TIP]
>
> To debug FTL, you can check its logs from the serial port:
>
> ```
> gcloud compute instances get-serial-port-output <INSTANCE_NAME>
> ```
>
> You can also use the `serial-port-logging-enable` feature to forward serial console logs to Cloud Logging.

## Firecracker

1. Build FTL in release mode:

```
RELEASE=1 ./build.sh
```

2. Create a Firecracker configuration file (`config.json`):

```json
{
  "boot-source": {
    "kernel_image_path": "ftl.elf",
    "initrd_path": "system.elf",
    "boot_args": "ftl.run=/bin/util ftl.arg=httpd"
  }
}
```

3. Start Firecracker:

```
firecracker --api-sock firecracker.sock --config-file config.json
```
