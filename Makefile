.PHONY: test
test:
	cargo watch -x test -i "*.result.pdf"

.PHONY: update_snapshots
update_snapshots:
	rename -fg 's/\.result\.pdf$$/\.pdf/' 'pdfrs/tests/fixtures/*.pdf'