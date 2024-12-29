Uzupełniona instrukcja dla developera

Project Overview

The primary goal of the Runner application is to download and launch a program called the Launcher. The Runner:
1. Downloads a Launcher package in ZIP format from a remote server.
2. Extracts the Launcher and its supporting files.
3. Reads a configuration file (patcher.manifest) to determine how to execute the Launcher.
4. Passes appropriate arguments to the Launcher during execution.
5. Manages lockfiles to prevent multiple instances of the Launcher.
6. Displays progress and error messages via a user interface.

Core Features (Updated)
	•	File Operations:
	•	Read a .dat file to retrieve the application’s secret key.
	•	Download and extract a ZIP package containing the Launcher.
	•	Parse and process the patcher.manifest file for:
	•	The executable to launch.
	•	Arguments to pass to the executable (e.g., --installdir, --lockfile, --secret, --network-status).
	•	Variables to resolve based on runtime conditions (e.g., {exedir}, {installdir}).
	•	Lockfile Management:
	•	Check for an existing lockfile and prevent multiple Launcher instances from running simultaneously.
	•	Display a clear error message if a lockfile is detected:
	•	Allow the user to manually delete the lockfile or wait until it expires.
	•	Error Handling:
	•	Display user-friendly error messages.
	•	Log all errors and application events.
	•	UI:
	•	Progress bar for download and extraction processes.
	•	Dialog box for errors, including specific instructions for resolving lockfile issues.

New Components

Additions to the architecture include a new class to handle the patcher.manifest file and lockfile logic:
	1.	ManifestManager:
	•	Responsible for parsing the patcher.manifest file.
	•	Resolves variables like {exedir}, {installdir}, {lockfile}, and {secret}.
	•	Provides methods to extract:
	•	Target executable path.
	•	Arguments for the Launcher.
	2.	LockfileManager:
	•	Manages the creation and validation of lockfiles.
	•	Ensures only one Launcher instance runs at a time.
	•	Deletes expired lockfiles.

Updated Flow
	1.	Initialization:
	•	The Runner starts and loads configuration from a .dat file.
	•	The ConfigManager retrieves the secret key.
	2.	Download and Extraction:
	•	The Runner queries the PatchKit API for the latest version and downloads the Launcher package.
	•	Extract the ZIP package to a subdirectory.
	3.	Manifest Processing:
	•	Parse patcher.manifest using ManifestManager.
	•	Resolve variables and prepare arguments for the Launcher.
	4.	Lockfile Validation:
	•	Before launching, check the lockfile using LockfileManager.
	•	If a valid lockfile exists:
	•	Inform the user about the lockfile.
	•	Allow them to wait, delete it manually, or cancel.
	5.	Launcher Execution:
	•	Execute the Launcher with resolved arguments.
	•	Close the Runner upon successful execution.
	6.	Error Handling:
	•	Display errors via UI and log them to a file.

Additional Notes
	1.	Manifest File Parsing:
	•	Use serde_json to parse the patcher.manifest file.
	•	Ensure proper validation of the JSON structure.
	2.	Lockfile Logic:
	•	If a lockfile exists and is younger than 60 seconds:
	•	Prevent launching the application.
	•	Display a message indicating the lockfile’s location and how to resolve the issue.
	•	If the lockfile is older, automatically delete it and proceed.
	3.	Testing Updates:
	•	Test ManifestManager to ensure correct parsing and variable resolution.
	•	Test LockfileManager for proper creation, validation, and deletion of lockfiles.
	•	Ensure error scenarios (e.g., invalid manifest, missing lockfile) are handled gracefully.

Does this address all the required updates, or should I elaborate further on any part?