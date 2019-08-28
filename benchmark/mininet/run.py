import sys

# print sys.path
# sys.path.remove('/usr/local/lib/python2.7/dist-packages')
# sys.path.remove(
#     '/usr/local/lib/python2.7/dist-packages/mininet-2.3.0d5-py2.7.egg')
# sys.path.remove(
#     '/usr/local/lib/python2.7/dist-packages/mininet_wifi-2.3-py2.7.egg')
# sys.path.append('/home/skye/play/mininet-wifi')
# sys.path.append('/home/skye/play/mininet-wifi/mininet')

from mininet.log import setLogLevel, info
from mininet.cli import CLI
from mininet.net import Mininet
from mininet.node import RemoteController
from rusty_bmv2 import RustyBmv2Switch

def topology():
    "Create a network."
    net = Mininet()
    ctrl = net.addController('c0', controller=RemoteController, ip="127.0.0.1", port=6653)

    info("*** Creating nodes\n")
    h1 = net.addHost("h1")
    h2 = net.addHost("h2")

    info("*** Creating switch")
    s1 = net.addSwitch("s1", cls=RustyBmv2Switch, grpcport=50051)

    info("*** Creating links\n")
    net.addLink(s1, h1)
    net.addLink(s1, h2)

    info("*** Starting network\n")
    net.build()
    s1.start([ctrl])

    h1.cmd('ethtool -K h1-eth0 gro off rx off tx off')
    h2.cmd('ethtool -K h2-eth0 gro off rx off tx off')
    info("*** Running CLI\n")
    CLI(net)

    info("*** Stopping network\n")
    net.stop()

if __name__ == '__main__':
    setLogLevel('debug')
    topology()
