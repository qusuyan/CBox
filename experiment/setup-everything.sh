#! /bin/bash

dist-make copycat all install-rust --engine=none
dist-make copycat all setup-mailbox
dist-make copycat all setup-copycat
