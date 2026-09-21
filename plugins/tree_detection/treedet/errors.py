"""Errors raised by Module 7.

All of them derive from ``PluginExecutionError`` so ``main.py`` can catch one
type, write a ``"failure"`` result, and exit without crashing the host.
"""


class PluginExecutionError(Exception):
    """The plugin cannot finish this run (e.g. the image file is missing)."""


class UnsupportedFormatError(PluginExecutionError):
    """The image format is not one the plugin can read."""


class InvalidGeometryError(PluginExecutionError):
    """A plot or AOI boundary is not a valid GeoJSON Polygon/MultiPolygon."""


class ZeroAreaError(PluginExecutionError):
    """Density was requested for an area of 0 m² (would divide by zero)."""
