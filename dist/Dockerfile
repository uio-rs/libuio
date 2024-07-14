FROM rust:1-bookworm

RUN rustup update

RUN apt-get update && apt-get install -yqq ninja-build gettext cmake unzip curl build-essential git

RUN git clone https://github.com/neovim/neovim \
  && cd neovim \
  && git checkout stable \
  && make CMAKE_BUILD_TYPE=RelWithDebInfo \
  && make install

RUN curl https://raw.githubusercontent.com/creationix/nvm/master/install.sh | bash

ENV NODE_VERSION=20.15.0
ENV NVM_DIR=/root/.nvm
RUN . "$NVM_DIR/nvm.sh" && nvm install ${NODE_VERSION}
RUN . "$NVM_DIR/nvm.sh" && nvm use v${NODE_VERSION}
RUN . "$NVM_DIR/nvm.sh" && nvm alias default v${NODE_VERSION}
ENV PATH="/root/.nvm/versions/node/v${NODE_VERSION}/bin/:${PATH}"

ENV GO_VERSION=1.22.5
RUN curl -LO https://go.dev/dl/go${GO_VERSION}.linux-arm64.tar.gz \
  && rm -rf /usr/local/go \
  && tar -C /usr/local -xzf go${GO_VERSION}.linux-arm64.tar.gz \
  && rm go${GO_VERSION}.linux-arm64.tar.gz

ENV RIPGREP_VERSION=14.1.0
RUN curl -LO https://github.com/BurntSushi/ripgrep/releases/download/${RIPGREP_VERSION}/ripgrep-${RIPGREP_VERSION}-aarch64-unknown-linux-gnu.tar.gz \
  && tar -xzf ripgrep-${RIPGREP_VERSION}-aarch64-unknown-linux-gnu.tar.gz \
  && mv ripgrep-${RIPGREP_VERSION}-aarch64-unknown-linux-gnu/rg /usr/local/bin \
  && rm -rf ripgrep-${RIPGREP_VERSION}-aarch64-unknown-linux-gnu.tar.gz ripgrep-${RIPGREP_VERSION}-aarch64-unknown-linux-gnu

ENV LAZYGIT_VERSION=0.42.0
RUN mkdir lazygit && cd lazygit \
  && curl -LO https://github.com/jesseduffield/lazygit/releases/download/v${LAZYGIT_VERSION}/lazygit_${LAZYGIT_VERSION}_Linux_arm64.tar.gz \
  && tar -xzf lazygit_${LAZYGIT_VERSION}_Linux_arm64.tar.gz \
  && mv lazygit /usr/local/bin \
  && cd .. && rm -rf lazygit

ENV GDU_VERSION=5.29.0
RUN curl -LO https://github.com/dundee/gdu/releases/download/v${GDU_VERSION}/gdu_linux_arm64.tgz \
  && tar -xvf gdu_linux_arm64.tgz \
  && chmod +x gdu_linux_arm64 \
  && mv gdu_linux_arm64 /usr/local/bin/gdu

RUN cargo install bottom --locked

RUN apt-get update && apt-get install -yqq python3 python3-pip xclip zsh

ENV PATH="${PATH}:/usr/local/go/bin"
ENV SHELL="/bin/zsh"

RUN rustup component add rust-analyzer clippy rustfmt
RUN sh -c "$(curl -fsSL https://raw.githubusercontent.com/ohmyzsh/ohmyzsh/master/tools/install.sh)"
RUN git clone --depth=1 https://github.com/romkatv/powerlevel10k.git ${ZSH_CUSTOM:-$HOME/.oh-my-zsh/custom}/themes/powerlevel10k

ENV FZF_VERSION=0.53.0
RUN mkdir fzf && cd fzf \
  && curl -LO https://github.com/junegunn/fzf/releases/download/${FZF_VERSION}/fzf-${FZF_VERSION}-linux_arm64.tar.gz \
  && tar -zxf fzf-${FZF_VERSION}-linux_arm64.tar.gz \
  && mv fzf /usr/local/bin \
  && cd .. && rm -rf fzf

COPY dist/.fzf.zsh /root/.fzf.zsh
WORKDIR /opt/libuio
