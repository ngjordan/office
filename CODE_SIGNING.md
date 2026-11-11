# Code signing policy

Free code signing provided by SignPath.io, certificate by SignPath Foundation.

The Windows artifacts of this repository - the installer and the portable archive - are signed
with a certificate issued to SignPath Foundation, which is therefore the publisher shown by
Windows. SignPath Foundation is not a certification authority: it holds the certificate on
behalf of qualifying open-source projects, and its name and reputation are on the line for
every signature it issues.

## What gets signed

Only the artifacts this repository builds from its own source, in its own CI:

- `iridium-office-<version>-windows-<arch>.msi` - the installer, together with the
  executable inside it.
- `iridium-office-<version>-windows-<arch>-portable.zip` - the portable archive, containing
  the signed executable.

Nothing else is signed. The macOS and Linux artifacts are signed with a Developer ID
certificate and an App Store Connect API key, which are not SignPath Foundation's business
and are not covered by this policy.

The five applications this suite embeds live in their own repositories and are not signed
with this certificate.

## Who may approve a release

Each signing request has to be approved by a member of the **Approvers** team before
SignPath signs it. The approval is the last check before a binary carries a signature that
points at this project, so it is made by someone who understands the release.

| Role | Who | What they do |
|---|---|---|
| Authors | the maintainers of this repository | change the source |
| Reviewers | the maintainers of this repository | review a change that is not their own |
| Approvers | the repository owners | approve a release for signing |

The repository is single-maintainer, so the same person fills all three roles. The rule that
matters is the separation of *writing* the code from *approving the signature*: a change that
is not your own gets read by someone else before it is signed.

## How a release is signed

1. The release workflow builds the artifacts on a GitHub-hosted runner.
2. The unsigned artifacts are uploaded as a workflow artifact, which is what SignPath checks:
   the build happened in the workflow, on a GitHub runner, from this repository.
3. The workflow submits a signing request. The action waits for the approval.
4. A member of the Approvers team approves it in SignPath.
5. SignPath signs the files and the workflow continues with the signed artifacts.

A local build is never signed: the signing action only accepts an artifact that a GitHub
workflow produced, so there is no path by which something that was not built here gets a
signature.

## Privacy

**The Office suite collects no data and transmits none.** It makes no network connection of
its own: there is no telemetry, no analytics, no crash reporting, and no automatic update
check.

The Sheets module can talk to Google Sheets and to Microsoft 365, but only when you ask it
to, using your own credentials, and it sends your data only to the service you chose. What
you open stays on your machine unless you send it somewhere yourself.

There is therefore no installation option to disable: there is nothing to disable.

## The rules this project follows

SignPath Foundation's conditions are in full at <https://signpath.org/terms.html>. In short:

- Every component is under an OSI-approved open-source licence (MIT or Apache-2.0).
- There is no proprietary or non-open-source component in a signed artifact.
- The project is actively maintained and released.
- This page describes what the software does.
- Everyone with access to the source repository or to SignPath uses multi-factor
  authentication.

If a rule is broken, SignPath Foundation can pause the subscription or revoke the
certificate, with immediate effect.
