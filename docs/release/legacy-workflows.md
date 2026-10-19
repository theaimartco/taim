# Archived legacy GitHub Actions workflows

The five `.github/workflows/{ci,fuzz,golden,licenses,perf}.yml` files
in the base tree contained **comments only** and did not execute their
declared jobs. They were removed from the workflows directory to prevent
invalid empty-job workflow noise. Their exact historical content is retained
in Git history at base commit `3f96da262948c3403280f58759a35fca1ded06e2`.

The supported replacement is `.github/workflows/verification.yml`, which
now owns formatting, compilation, tests, visual goldens, AT-SPI, dependency
audit, the budget test, package inventory, external downstream compile and
scheduled fuzzing.

Do **not** infer that a workflow is green if the jobs did not actually start.
Check every job's steps and the GitHub Actions runner allocation status.
None of these hosted gates certify a physical VYOM GPU presentation, measured
input-to-photon latency, external screen reader functionality or signed release.
