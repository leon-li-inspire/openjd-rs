This is a Docker container that sets up a Linux environment with LDAP-based
user management for testing cross-user functionality in `openjd-sessions`.
Users are provisioned via OpenLDAP rather than local `/etc/passwd` entries.

## Build
```
docker build --build-arg BUILDKIT_SANDBOX_HOSTNAME=ldap.environment.internal --ulimit nofile=1024 -t openjd_rs_ldap_test -f testing_containers/ldap_sudo_environment/Dockerfile .
```

## Run
```
docker run -h ldap.environment.internal --rm openjd_rs_ldap_test:latest
```

## Interactive
```
docker run -h ldap.environment.internal --rm -it openjd_rs_ldap_test:latest bash
/config/start_ldap.sh
```
