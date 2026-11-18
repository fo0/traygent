# Sourced by the container init. Keep writable tools off the root shell's PATH.
install_gh() {
[ ! -L /home/node/.local/bin ] && mkdir -p /home/node/.local/bin && chown -h node:node /home/node/.local/bin &&
runuser -u node -- env PATH="/home/node/.local/bin:$PATH" bash -s <<'TRAYGENT_GH'
set -eu
if gh --version >/dev/null 2>&1; then exit 0; fi
cd /home/node/.local/bin
exec 8<.
flock -x 8
if gh --version >/dev/null 2>&1; then exit 0; fi
echo '[traygent] Installiere GitHub CLI ...'
version=2.104.0
case "$(uname -m)" in
  x86_64) arch=amd64; sha=98cb101ad87dbbd3f6b2f2bfcbd43197ccd07643140121d8a9cfaa91eadbc24e ;;
  aarch64) arch=arm64; sha=cb34cefc6f4e77b90731374a2b737516bb8f3033c6ca46aa949e5344f68069e3 ;;
  i?86) arch=386; sha=c786883d42b71a305618a0b9ac129af2b90f27063106b3ba7ca185fea66da972 ;;
  armv[67]l) arch=armv6; sha=2fae67a4d3bc21567edb47f9ad19086c98af88a9cb8ffea56a5c85c6e5eb7c7a ;;
  *) echo '[traygent] Keine GitHub CLI für diese Prozessorarchitektur.'; exit 1 ;;
esac
asset=gh_${version}_linux_${arch}
work=$(mktemp -d .gh-install.XXXXXXXX)
trap 'rm -f "$work/archive" "$work/gh"; rmdir "$work"' EXIT
curl -fsSL --connect-timeout 10 --max-time 120 --retry 1 -o "$work/archive" "https://github.com/cli/cli/releases/download/v$version/$asset.tar.gz"
printf '%s  %s\n' "$sha" "$work/archive" | sha256sum -c --status
tar -xzOf "$work/archive" "$asset/bin/gh" >"$work/gh"
chmod 755 "$work/gh"
"$work/gh" --version >/dev/null
mv -fT "$work/gh" gh
TRAYGENT_GH
}
if ! install_gh; then
  echo '[traygent] GitHub CLI ist nicht verfügbar. Installation beim nächsten Start erneut versuchen.'
  if [ "${GIT_CONFIG_KEY_3:-}" = 'credential.https://github.com.helper' ] && [ -n "${GH_TOKEN:-}" ]; then
    export GIT_CONFIG_VALUE_3='!f() { if [ "$1" = get ]; then printf "%s\n" username=x-access-token "password=$GH_TOKEN"; fi; }; f'
  fi
fi
