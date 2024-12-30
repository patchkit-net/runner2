## Single-instance guard

The runner checks if another instance of itself is running. If it is, it will exit.

## Online checks

The runner should perform a single online check to determine if we're connected to the internet.

1. Execute a GET request to https://network-test.patchkit.net/
2. The request should respond with 200 OK and a "ok" string in the response body.
3. If the request fails, the runner should display a dialog box with the message "No internet connection" (think of something better) with options to retry, enter offline mode or exit.

## Lockfile management

The runner should check for an existing lockfile. If it exists, it should display a dialog box with the message "Another instance of the launcher is already running. Please wait for it to finish or delete the lockfile manually." with options to delete the lockfile or wait until it expires.

If the lockfile does not exist, the runner should create it.

## Offline mode

The runner should enter offline mode if the user chooses to do so. This should pass the --network-status=offline argument to the launcher.

## Running the downloaded launcher application

Runner downloads a zip file and unpacks it to a directory. Then, it looks for a manifest file. It's called "patcher.manifest" and looks like this:

```json
{
  "exe_fileName": "\"{exedir}/Patcher.exe\"",
  "exe_arguments": "--installdir \"{installdir}\" --secret \"{secret}\"",
  "manifest_version": 4,
  "target": "{exedir}/Patcher.exe",
  "target_arguments": [
    {
      "value": [
        "--installdir",
        "{installdir}"
      ]
    },
    {
      "value": [
        "--lockfile",
        "{lockfile}"
      ]
    },
    {
      "value": [
        "--secret",
        "{secret}"
      ]
    },
    {
      "value": [
        "--{network-status}"
      ]
    }
  ],
  "capabilities": [
    "pack1_compression_lzma2",
    "security_fix_944",
    "preselected_executable",
    "execution_arguments",
    "changelog_endpoint_2033"
  ]
}
```

- exe_fileName and exe_arguments are deprecated and should be ignored.
- target is the path to the executable to run.
- target_arguments are the arguments to pass to the executable.
- capabilities should be ignored.

The runner should run the executable with the arguments, setting the values of the variables:

- exedir - the directory where the runner has unpacked the downloaded launcher application.
- installdir - the directory where the downloaded launcher will install the target game files.
- secret - the secret that the runner has read from the dat file.
- lockfile - the lockfile that the runner has generated. This will pass the lockfile ownership to the launcher.
- network-status - the network status that the runner has determined. This should be "online" or "offline".

# Debugging environment variables

You can use the debugging environment variables to force a specific endpoint, disable the lockfile, enter offline mode, change the endpoint. When any environment variable is set, a warning message is displayed in the UI, asking the user if they want to continue, because if that's not their doing, it may be a security risk.

- `PK_RUNNER_API_URL` - force a specific endpoint.
- `PK_RUNNER_DISABLE_LOCKFILE` - disable the lockfile.
- `PK_RUNNER_OFFLINE` - enter offline mode.
- `PK_RUNNER_ENDPOINT` - change the endpoint.

## Error Handling

All errors should present a user-friendly dialog with:
1. Specific error description
2. "Try Again" and "Cancel" options

Example error scenarios and messages:

```
Network Error
Could not connect to PatchKit servers. Please check your internet connection.
[Try Again] [Cancel]

Installation Error
Could not create installation directory. Please check permissions.
[Try Again] [Cancel]

Version Error
Failed to download latest version. Server returned: 404
[Try Again] [Cancel]
```
# Endpoints:

The default host is `https://api2.patchkit.net/`.

## Application Version Endpoint:

"/1/apps/{SECRET}/versions/latest/id"

It returns a json with the "id" field of the latest version (VERSION_ID)

## Application Download Endpoint:

"/1/apps/{SECRET}/versions/{VERSION_ID}/content_urls"

It returns a json with the "url" field of the content urls:

```json
[
    {
        "size": 1563832028,
        "url": "https://cdn-cf-ae.patchkit.net/resources/abc"
    }
]

The URL is the direct url to a zip file that contains the launcher application.
