.PHONY: devel
devel: build volumes
	docker run --rm -it --name libuio-devel --net host \
        -v "nvim-cache:/root/.cache/nvim" \
        -v "nvim-state:/root/.local/state" \
        -v "nvim-share:/root/.local/share" \
        -v "${HOME}/.config/nvim:/root/.config/nvim" \
        -v "${HOME}/.ssh:/root/.ssh:ro" \
        -v "${HOME}/.gnupg:/root/.gnupg:ro" \
        -v "${PWD}:/opt/libuio" \
        -v "${HOME}/.zshrc:/root/.zshrc:ro" \
        -v "${HOME}/.zshenv:/root/.zshenv:ro" \
        -v "${HOME}/.p10k.zsh:/root/.p10k.zsh:ro" \
        --privileged \
        --entrypoint nvim \
        libuio-devel:latest /opt/libuio

.PHONY: build
build:
	 docker build -t libuio-devel:latest -f dist/Dockerfile .

.PHONY: volumes
volumes:
	docker volume create nvim-cache
	docker volume create nvim-state
	docker volume create nvim-share
