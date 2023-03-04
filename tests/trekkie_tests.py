import pytest
import requests
import logging
import http.client as http_client

OFFLINE = True
VERBOSE = False

OFFLINE_HOST = "http://localhost:8060"
STAGING_HOST = "https://trekkie.staging.dvb.solutions"

HOST = OFFLINE_HOST if OFFLINE else STAGING_HOST

files = {"upload_file": open("test.gpx", "rb")}

times_json = {
    "gpx_id": "fillme",
    "vehicles": [
        {
            "start": "2022-09-10T14:46:30.290072949",
            "stop": "2022-09-10T15:16:25.754147203",
            "line": 63,
            "run": 8,
            "region": 0,
        },
        {
            "start": "2022-09-10T15:22:30.290072949",
            "stop": "2022-09-10T15:29:25.754147203",
            "line": 7,
            "run": 27,
            "region": 0,
        },
    ],
}

# this enables higly verbose logging for debug purposes
# http_client.HTTPConnection.debuglevel = 1
# logging.basicConfig()
# logging.getLogger().setLevel(logging.DEBUG)
# requests_log = logging.getLogger("requests.packages.urllib3")
# requests_log.setLevel(logging.DEBUG)
# requests_log.propagate = True


class TestTrekkie:
    session = requests.Session()

    @pytest.mark.run(order=0)
    def test_create_user(self):
        create_user_response = self.session.post(HOST + "/user/create")
        assert create_user_response.status_code == 200

    @pytest.mark.run(order=1)
    def test_submit_run(self):
        submit_gpx = self.session.post(HOST + "/travel/submit/gpx", files=files)
        assert submit_gpx.status_code == 200

        times_json["gpx_id"] = submit_gpx.json()["gpx_id"]
        submit_run = self.session.post(HOST + "/travel/submit/run", json=times_json)

        assert submit_gpx.status_code == 200

    @pytest.mark.run(order=3)
    def test_list_runs(self):
        list_run = self.session.get(HOST + "/travel/submit/list")

        assert list_run.status_code == 200

