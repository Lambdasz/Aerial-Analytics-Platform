"""Tests for the pure parts of the training tools in training/.

Downloading and fine-tuning need the network and deepforest, so only the logic around
them is tested: reading a WebDAV listing, picking photos and tiles, splitting by photo,
and the Pascal VOC / DeepForest CSV formats.

Run from the repository root:

    python -m unittest discover -s tests -v
"""

import unittest

from training.fetch_visloc import RemoteFile, evenly_spaced, parse_listing, share_of
from training.train_deepforest import (
    Box,
    Tile,
    csv_rows,
    grid,
    is_validation,
    parse_voc,
    photo_of,
    spread,
    voc_xml,
)

LISTING = """<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:">
  <d:response><d:href>/public.php/webdav/UAV_VisLoc_dataset/07/</d:href>
    <d:propstat><d:prop><d:quota-used-bytes>30549205</d:quota-used-bytes></d:prop></d:propstat>
  </d:response>
  <d:response><d:href>/public.php/webdav/UAV_VisLoc_dataset/07/07.csv</d:href>
    <d:propstat><d:prop><d:getcontentlength>1877</d:getcontentlength></d:prop></d:propstat>
  </d:response>
  <d:response><d:href>/public.php/webdav/UAV_VisLoc_dataset/07/drone/</d:href>
    <d:propstat><d:prop><d:quota-used-bytes>28187546</d:quota-used-bytes></d:prop></d:propstat>
  </d:response>
  <d:response><d:href>/public.php/webdav/UAV_VisLoc_dataset/07/satellite%2007.tif</d:href>
    <d:propstat><d:prop><d:getcontentlength>2359782</d:getcontentlength></d:prop></d:propstat>
  </d:response>
</d:multistatus>"""


class FetchTest(unittest.TestCase):
    def test_share_link_becomes_webdav_endpoint(self) -> None:
        share = share_of("https://cloud.example/s/abc123?dir=/x")
        self.assertEqual(share.webdav_root, "https://cloud.example/public.php/webdav")
        self.assertEqual(share.authorization, "Basic YWJjMTIzOg==")  # "abc123:"

    def test_a_link_that_is_not_a_share_is_rejected(self) -> None:
        with self.assertRaises(ValueError):
            share_of("https://cloud.example/index.php/apps/files")

    def test_listing_keeps_files_only(self) -> None:
        files = parse_listing(LISTING, "/public.php/webdav")
        self.assertEqual(
            files,
            (
                RemoteFile("UAV_VisLoc_dataset/07/07.csv", 1877),
                RemoteFile("UAV_VisLoc_dataset/07/satellite 07.tif", 2359782),
            ),
        )

    def test_photos_are_spread_over_the_flight(self) -> None:
        photos = [RemoteFile(f"{i:04d}.JPG", 1) for i in range(100)]
        picked = evenly_spaced(photos, 4)
        self.assertEqual([p.path for p in picked], ["0000.JPG", "0025.JPG", "0050.JPG", "0075.JPG"])
        self.assertEqual(len(evenly_spaced(photos, 0)), 100)


class TilingTest(unittest.TestCase):
    def test_grid_keeps_only_full_tiles(self) -> None:
        self.assertEqual(grid(900, 450, 400), (Tile(0, 0), Tile(400, 0)))

    def test_spread_returns_everything_when_asked_for_more(self) -> None:
        self.assertEqual(spread((1, 2, 3), 10), (1, 2, 3))

    def test_tile_name_leads_back_to_its_photo(self) -> None:
        self.assertEqual(photo_of("01_0050__x400_y0.jpg"), "01_0050")

    def test_split_is_stable_and_by_photo(self) -> None:
        self.assertEqual(is_validation("01_0050", 0.2), is_validation("01_0050", 0.2))
        self.assertFalse(is_validation("01_0050", 0.0))
        self.assertTrue(is_validation("01_0050", 1.0))


class LabelFormatTest(unittest.TestCase):
    def test_voc_round_trip(self) -> None:
        boxes = (Box(1, 2, 30, 40), Box(100, 120, 180, 200))
        self.assertEqual(parse_voc(voc_xml("t.jpg", 400, boxes)), ("t.jpg", boxes))

    def test_degenerate_boxes_are_ignored(self) -> None:
        self.assertEqual(parse_voc(voc_xml("t.jpg", 400, (Box(5, 5, 5, 9),))), ("t.jpg", ()))

    def test_csv_rows(self) -> None:
        self.assertEqual(list(csv_rows("t.jpg", (Box(1, 2, 3, 4),))), ["t.jpg,1,2,3,4,Tree"])

    def test_tile_without_trees_is_an_empty_frame(self) -> None:
        self.assertEqual(list(csv_rows("t.jpg", ())), ["t.jpg,0,0,0,0,Tree"])
