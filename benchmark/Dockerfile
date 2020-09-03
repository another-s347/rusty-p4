
FROM p4lang/behavioral-model:latest

USER root
WORKDIR /root

RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    iproute2 \
    iputils-ping \
    net-tools \
    openvswitch-switch \
    openvswitch-testcontroller \
    tcpdump \
    vim \
    x11-xserver-utils \
    xterm \
    git \
 && rm -rf /var/lib/apt/lists/*

RUN apt-get update && apt-get install -y --no-install-recommends \ 
    sudo

RUN export HTTP_PROXY=http://172.17.0.1:8118 && export HTTPS_PROXY=http://172.17.0.1:8118 && git clone https://github.com/intrig-unicamp/mininet-wifi
RUN apt-get install -y --no-install-recommends \ 
    python3-pip python-pip
RUN update-alternatives --install /usr/bin/python python /usr/bin/python3 0
# RUN pip3 config set global.index-url https://pypi.tuna.tsinghua.edu.cn/simple
RUN unset HTTP_PROXY && unset HTTPS_PROXY
RUN cd mininet-wifi \
    # && export http_proxy=http://172.17.0.1:8118 && export https_proxy=http://172.17.0.1:8118 \
    && util/install.sh -Wlnfv

RUN mkdir rusty
COPY run.py /root/rusty
COPY rusty_bmv2.py /root/rusty