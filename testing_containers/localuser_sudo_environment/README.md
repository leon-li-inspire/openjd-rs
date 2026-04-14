This is a Docker container that sets up a Linux environment for testing
cross-user functionality in `openjd-sessions`. It creates local users
(`hostuser`, `targetuser`, `disjointuser`) with appropriate group
memberships and sudo permissions.

## Build
```
docker build -t openjd_rs_localuser_test -f testing_containers/localuser_sudo_environment/Dockerfile .
```

## Run
```
docker run --rm openjd_rs_localuser_test:latest
```

## Interactive
```
docker run --rm -it openjd_rs_localuser_test:latest bash
```
