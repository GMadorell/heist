.PHONY: reinstall

reinstall:
	claude plugin remove heist@heist-marketplace
	claude plugin install heist@heist-marketplace
	cargo uninstall heist || true
	cargo install --path cli
