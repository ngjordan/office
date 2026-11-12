# Applying to SignPath Foundation

This is the checklist for the free SignPath account and the Foundation programme. Everything
in this repository is already in place; what follows is what happens on signpath.org.

## What is in place already

| Requirement | Where |
|---|---|
| The repository is public | `github.com/ngjordan/office` |
| An OSI-approved licence | `LICENSE` (MIT), `LICENSE-APACHE` (Apache-2.0) |
| The project is released | the v0.1.0 release, with a download |
| The functionality is documented | `README.md` |
| The code signing policy | `README.md` and `CODE_SIGNING.md` |
| The artifact configuration | `packaging/windows/signpath/office-suite.xml` |
| The CI integration | `.github/workflows/release.yml`, the *Sign with SignPath* step |
| No proprietary component | the six apps are MIT/Apache, the assets are OFL/Apache |
| A privacy policy | `CODE_SIGNING.md`: the suite collects nothing and phones nowhere |

## On signpath.org

1. **Create the account.** `signpath.io`, *Sign up*. A free account for open-source projects.
2. **Enable multi-factor authentication** on the SignPath account, and on the GitHub account.
   Both are required, and the check is done on the people, not the project.
3. **Create the organisation.** One organisation for Iridium Technologies, Inc.
4. **Apply to the SignPath Foundation programme.** The application asks for:
   - the project: **Office**, the URL `https://github.com/ngjordan/office`
   - the licence: **MIT** (the Apache-2.0 parts can be listed as well)
   - a description of the software and of what it does
   - the URL of the code signing policy: `https://github.com/ngjordan/office/blob/main/CODE_SIGNING.md`
   - the people who can commit, who can review and who can approve, with their GitHub links
   - proof that the project is public and active
5. **Wait for the review.** The Foundation asks for public signs of trust - stars, forks,
   contributors, articles, sustained activity. This is the part to prepare for: the six
   repositories are public and have real history, but they are new and have no stars yet.

## Once it is approved

In the SignPath project:

6. **Create the signing policy.** *Signing policies* describes who may approve a request.
   Point it at the people in the code signing policy.
7. **Create the artifact configuration.** Paste `office-suite.xml`, or upload a sample
   artifact and let SignPath generate one, then compare it with the file in this directory.
8. **Create the API token.** *Account settings*, an API token with the right scopes, and
   store it as the repository secret `SIGNPATH_API_TOKEN`.
9. **Set the repository variables.** *Settings*, *Secrets and variables*, *Actions*,
   *Variables*, in the `release` environment:

   | Variable | Where to find it |
   |---|---|
   | `SIGNPATH_ORGANIZATION_ID` | the organisation's page |
   | `SIGNPATH_PROJECT_SLUG` | the project's URL |
   | `SIGNPATH_POLICY_SLUG` | the signing policy's URL |
   | `SIGNPATH_ARTIFACT_CONFIGURATION` | the artifact configuration's slug |

10. **Run a release.** The *Sign with SignPath* step submits the request and waits. Approve
    it in SignPath and the run continues.

## If the application is declined

The Foundation says it will invite a new application once the project has more public
signals. That is the one thing that cannot be built in an afternoon: stars, forks,
contributors and articles have to come from other people.

The alternatives, in the order they cost:

| Route | Cost | Publisher shown |
|---|---|---|
| Microsoft Store, MSIX | free | none - the Store re-signs |
| Certum open source | EUR 49/yr | *Open Source Developer* + your name; individuals only, non-commercial only |
| SSL.com OV | $129/yr | your name or your company |
| Azure Artifact Signing | ~$10/mo | your validated identity |
