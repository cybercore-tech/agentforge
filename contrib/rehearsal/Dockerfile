# Two-host rehearsal image (P4-M012): a clean Arch Linux host for either role.
# Used by scripts/rehearse-two-hosts; forge, forged, and ghostport are installed by the script.
FROM archlinux:latest
RUN pacman -Syu --noconfirm --needed git iproute2 iputils procps-ng \
 && pacman -Scc --noconfirm \
 && useradd -m coord \
 && useradd -m worker
COPY rehearsal-agent /usr/local/bin/rehearsal-agent
RUN chmod 755 /usr/local/bin/rehearsal-agent
