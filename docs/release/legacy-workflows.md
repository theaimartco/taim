# Archived legacy GitHub Actions workflows

The five `.github/workflows/{ci,fuzz,golden,licenses,perf}.yml` files
in the base tree contained **comments only** and did not execute their
declared jobs. They were removed from the workflows directory to prevent
invalid empty-job workflow noise. Their exact historical content is retained
in Git history at base commit `b9cd78784a8ede572b1d688c2808e0478bdecec1`.

The supported replacement is `.github/workflows/verification.yml`, which
now owns formatting, compilation, tests, visual goldens, AT-SPI, dependency
audit, the budget test, package inventory, external downstream compile and
scheduled fuzzing.

Do **not** infer that a workflow is green if the jobs did not actually start.
Check every job's steps and the GitHub Actions runner allocation status.
None of these hosted gates certify a physical VYOM GPU presentation, measured
input-to-photon latency, external screen reader functionality or signed release.
