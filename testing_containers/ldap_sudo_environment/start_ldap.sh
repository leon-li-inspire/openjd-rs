#!/bin/bash
# Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.

set -eo

rm -f /run/slapd/slapd.pid /run/nslcd/nslcd.pid
mkdir -p /run/nscd
/usr/sbin/slapd -h 'ldap:/// ldapi:///' -u openldap -g openldap
/usr/sbin/nslcd
/usr/sbin/nscd
