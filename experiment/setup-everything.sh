#! /bin/bash

dist-make copycat all setup-env --engine=none
dist-make copycat all install-rust --engine=none
dist-make copycat all setup-mailbox -Dbranch=dev
dist-make copycat all setup-copycat -Dbranch=dev
