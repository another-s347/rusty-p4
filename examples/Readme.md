# Examples with rusty_p4

## Prepare

- Generate pipeconf files from .p4 file

The extension must be '.bin' instead of '.txt'.
```
p4c-bm2-ss --p4v 16 --p4runtime-files my_pipeconf.p4.p4info.bin -o build/my_pipeconf.json my_pipeconf.p4
```

- Start your network with bmv2.

For example, using docker image from opennetworking, which is very handy.
```
docker run --privileged --rm -it -p 50001-50003:50001-50003 opennetworking/p4m
```
You may want to disable the checksum offload, otherwise TCP or UDP packets would be dropped by kernel.
```
mininet> h1 ethtool --offload h1-eth0 tx off
mininet> h1 ethtool --offload h1-eth0 rx off
```

## bmv2_conn
- Connect to a bmv2 switch at 172.17.0.2 using Bmv2SwitchConnection.
- Process p4 stream response.

## forward_app
- A simple forward app with dependency.
- Use flow macro and write flow table.
- Send packet.