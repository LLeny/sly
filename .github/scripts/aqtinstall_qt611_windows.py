#!/usr/bin/env python3

from aqt import main
from aqt.archives import QtArchives
from aqt.metadata import Version


_original_get_archives = QtArchives._get_archives


def _get_archives(self: QtArchives) -> None:
    if (
        self.os_name == "windows"
        and self.target == "desktop"
        and self.version >= Version("6.11.0")
    ):
        version = self._version_str()
        architecture = self.arch.removeprefix("win64_")
        repository = f"qt{self.version.major}_{version}/qt{self.version.major}_{version}_{architecture}"
        self._get_archives_base(repository, self._target_packages())
        return

    _original_get_archives(self)


QtArchives._get_archives = _get_archives

raise SystemExit(main())
