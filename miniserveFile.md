For when you really just want to serve some files over HTTP right now!

Usage: miniserve [OPTIONS] [PATH]

Arguments:
  [PATH]
          Which path to serve

          [env: MINISERVE_PATH=]

Options:
  -v, --verbose
          Be verbose, includes emitting access logs

          [env: MINISERVE_VERBOSE=]

      --temp-directory <TEMP_UPLOAD_DIRECTORY>
          The path to where file uploads will be written to before being moved to their correct location. It's wise to make sure that this
          directory will be written to disk and not into memory.

          This value will only be used **IF** file uploading is enabled. If this option is not set, the operating system default temporary
          directory will be used.

          [env: MINISERVER_TEMP_UPLOAD_DIRECTORY=]

      --index <INDEX>
          The name of a directory index file to serve, like "index.html"

          Normally, when miniserve serves a directory, it creates a listing for that directory. However, if a directory contains this file,
          miniserve will serve that file instead.

          [env: MINISERVE_INDEX=]

      --spa
          Activate SPA (Single Page Application) mode

          This will cause the file given by --index to be served for all non-existing file paths. In effect, this will serve the index file
          whenever a 404 would otherwise occur in order to allow the SPA router to handle the request instead.

          [env: MINISERVE_SPA=]

      --quiet
          Reduce output and silence warnings

          [env: MINISERVE_QUIET=]

      --pretty-urls
          Activate Pretty URLs mode

          This will cause the server to serve the equivalent `.html` file indicated by the path.

          `/about` will try to find `about.html` and serve it.

          [env: MINISERVE_PRETTY_URLS=]

  -p, --port <PORT>
          Port to use

          [env: MINISERVE_PORT=]
          [default: 8080]

  -i, --interfaces <INTERFACES>
          Interface to listen on

          [env: MINISERVE_INTERFACE=]

      --workers <WORKERS>
          Number of server workers

          [env: MINISERVE_WORKERS=]
          [default: 4]

  -a, --auth <AUTH>
          Set authentication

          Currently supported formats:
          username:password, username:sha256:hash, username:sha512:hash
          (e.g. joe:123, joe:sha256:a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3)

          [env: MINISERVE_AUTH=]

      --auth-file <AUTH_FILE>
          Read authentication values from a file

          Example file content:

          joe:123
          bob:sha256:a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3
          bill:

          [env: MINISERVE_AUTH_FILE=]

      --route-prefix <ROUTE_PREFIX>
          Use a specific route prefix

          [env: MINISERVE_ROUTE_PREFIX=]

      --random-route
          Generate a random 6-hexdigit route

          [env: MINISERVE_RANDOM_ROUTE=]

  -P, --no-symlinks
          Hide symlinks in listing and prevent them from being followed

          [env: MINISERVE_NO_SYMLINKS=]

  -H, --hidden
          Show hidden files

          [env: MINISERVE_HIDDEN=]

  -S, --default-sorting-method <DEFAULT_SORTING_METHOD>
          Default sorting method for file list

          Possible values:
          - name: Sort by name
          - size: Sort by size
          - date: Sort by last modification date (natural sort: follows alphanumerical order)

          [env: MINISERVE_DEFAULT_SORTING_METHOD=]
          [default: name]

  -O, --default-sorting-order <DEFAULT_SORTING_ORDER>
          Default sorting order for file list

          Possible values:
          - asc:  Ascending order
          - desc: Descending order

          [env: MINISERVE_DEFAULT_SORTING_ORDER=]
          [default: desc]

  -c, --color-scheme <COLOR_SCHEME>
          Default color scheme

          [env: MINISERVE_COLOR_SCHEME=]
          [default: squirrel]
          [possible values: squirrel, archlinux, ayu-dark, zenburn, monokai]

  -d, --color-scheme-dark <COLOR_SCHEME_DARK>
          Default color scheme

          [env: MINISERVE_COLOR_SCHEME_DARK=]
          [default: archlinux]
          [possible values: squirrel, archlinux, ayu-dark, zenburn, monokai]

  -q, --qrcode
          Enable QR code display

          [env: MINISERVE_QRCODE=]

  -u, --upload-files [<ALLOWED_UPLOAD_DIR>]
          Enable file uploading (and optionally specify for which directory)

          The provided path is not a physical file system path. Instead, it's relative to the serve dir. For instance, if the serve dir is
          '/home/hello', set this to '/upload' to allow uploading to '/home/hello/upload'. When specified via environment variable, a path
          always needs to be specified.

          [env: MINISERVE_ALLOWED_UPLOAD_DIR=]

      --web-upload-files-concurrency <WEB_UPLOAD_CONCURRENCY>
          Configure amount of concurrent uploads when visiting the website. Must have upload-files option enabled for this setting to matter.

          For example, a value of 4 would mean that the web browser will only upload 4 files at a time to the web server when using the web
          browser interface.

          When the value is kept at 0, it attempts to resolve all the uploads at once in the web browser.

          NOTE: Web pages have a limit of how many active HTTP connections that they can make at one time, so even though you might set a
          concurrency limit of 100, the browser might only make progress on the max amount of connections it allows the web page to have open.

          [env: MINISERVE_WEB_UPLOAD_CONCURRENCY=]
          [default: 0]

      --chmod <CHMOD>
          Set unix file permissions of uploaded files

          This takes an octal number, for example 0600. By default 0666 & ~umask is used to simulate the system's default behavior.

          [env: MINISERVE_CHMOD=]

      --directory-size
          Enable recursive directory size calculation

          This is disabled by default because it is a potentially fairly IO intensive operation.

          [env: MINISERVE_DIRECTORY_SIZE=]

  -U, --mkdir
          Enable creating directories

          [env: MINISERVE_MKDIR_ENABLED=]

      --pastebin
          Enable creating pastebin 'pastes'

          'pastes' are plaintext files created in the current directory. Creation requires file uploads be enabled.

          [env: MINISERVE_PASTEBIN_ENABLED=]

  -m, --media-type <MEDIA_TYPE>
          Specify uploadable media types

          [env: MINISERVE_MEDIA_TYPE=]
          [possible values: image, audio, video]

  -M, --raw-media-type <MEDIA_TYPE_RAW>
          Directly specify the uploadable media type expression

          [env: MINISERVE_RAW_MEDIA_TYPE=]

  -o, --on-duplicate-files <ON_DUPLICATE_FILES>
          What to do if existing files with same name is present during file upload

          If you enable renaming files, the renaming will occur by adding a numerical suffix to the filename before the final extension. For
          example file.txt will be uploaded as file-1.txt, the number will be increased until an available filename is found.

          [env: MINISERVE_ON_DUPLICATE_FILES=]
          [default: error]
          [possible values: error, overwrite, rename]

  -R, --rm-files [<ALLOWED_RM_DIR>]
          Enable file and directory deletion (and optionally specify for which directory)

          [env: MINISERVE_ALLOWED_RM_DIR=]

  -r, --enable-tar
          Enable uncompressed tar archive generation

          [env: MINISERVE_ENABLE_TAR=]

  -g, --enable-tar-gz
          Enable gz-compressed tar archive generation

          [env: MINISERVE_ENABLE_TAR_GZ=]

  -z, --enable-zip
          Enable zip archive generation

          WARNING: Zipping large directories can result in out-of-memory exception because zip generation is done in memory and cannot be sent
          on the fly

          [env: MINISERVE_ENABLE_ZIP=]

  -C, --compress-response
          Compress response

          WARNING: Enabling this option may slow down transfers due to CPU overhead, so it is disabled by default.

          Only enable this option if you know that your users have slow connections or if you want to minimize your server's bandwidth usage.

          [env: MINISERVE_COMPRESS_RESPONSE=]

  -D, --dirs-first
          List directories first

          [env: MINISERVE_DIRS_FIRST=]

  -t, --title <TITLE>
          Shown instead of host in page title and heading

          [env: MINISERVE_TITLE=]

      --header <HEADER>
          Inserts custom headers into the responses. Specify each header as a 'Header:Value' pair. This parameter can be used multiple times
          to add multiple headers.

          Example: --header "Header1:Value1" --header "Header2:Value2" (If a header is already set or previously inserted, it will not be
          overwritten.)

          [env: MINISERVE_HEADER=]

  -l, --show-symlink-info
          Visualize symlinks in directory listing

          [env: MINISERVE_SHOW_SYMLINK_INFO=]

  -F, --hide-version-footer
          Hide version footer

          [env: MINISERVE_HIDE_VERSION_FOOTER=]

      --hide-theme-selector
          Hide theme selector

          [env: MINISERVE_HIDE_THEME_SELECTOR=]

  -W, --show-wget-footer
          If enabled, display a wget command to recursively download the current directory

          [env: MINISERVE_SHOW_WGET_FOOTER=]

      --print-completions <shell>
          Generate completion file for a shell

          [possible values: bash, elvish, fish, powershell, zsh]

      --print-manpage
          Generate man page

      --tls-cert <TLS_CERT>
          TLS certificate to use

          [env: MINISERVE_TLS_CERT=]

      --tls-key <TLS_KEY>
          TLS private key to use

          [env: MINISERVE_TLS_KEY=]

      --readme
          Enable README.md rendering in directories

          [env: MINISERVE_README=]

  -I, --disable-indexing
          Disable indexing

          This will prevent directory listings from being generated and return an error instead.

          [env: MINISERVE_DISABLE_INDEXING=]

      --enable-webdav
          Enable read-only WebDAV support (PROPFIND requests)

          [env: MINISERVE_ENABLE_WEBDAV=]

      --size-display <SIZE_DISPLAY>
          Show served file size in exact bytes

          [env: MINISERVE_SIZE_DISPLAY=]
          [default: human]
          [possible values: human, exact]

      --file-external-url <FILE_EXTERNAL_URL>
          Optional external URL (e.g., 'http://external.example.com:8081') prepended to file links in listings.

          Allows serving files from a different URL than the browsing instance. Useful for setups like: one authenticated instance for
          browsing, linking files (via this option) to a second, non-indexed (-I) instance for direct downloads. This obscures the full file
          list on the download server, while users can still copy direct file URLs for sharing. The external URL is put verbatim in front of
          the relative location of the file, including the protocol. The user should take care this results in a valid URL, no further checks
          are being done.

          [env: MINISERVE_FILE_EXTERNAL_URL=]

      --log-color <LOG_COLOR>
          Set the color style of the log output

          "auto" (default) enables colors only when the output is a terminal. "always" always enables colors. "never" always disables colors.

          [env: MINISERVE_LOG_COLOR=]
          [default: auto]
          [possible values: auto, always, never]

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version