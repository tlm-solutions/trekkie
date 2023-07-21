import requests
import logging
import http.client as http_client

OFFLINE = False
VERBOSE = False

OFFLINE_HOST = "http://localhost:8060"
STAGING_HOST = "https://trekkie.borken.tlm.solutions"

HOST = OFFLINE_HOST if OFFLINE else STAGING_HOST

files = {"upload_file": open("test.gpx", "rb")}

init_json =  {
            "line": 63,
            "run": 8,
            "region": 0,
            "app_commit": "EEEEE",
            "app_name": "test"
}

# this enables higly verbose logging for debug purposes
http_client.HTTPConnection.debuglevel = 1
logging.basicConfig()
logging.getLogger().setLevel(logging.DEBUG)
requests_log = logging.getLogger("requests.packages.urllib3")
requests_log.setLevel(logging.DEBUG)
requests_log.propagate = True

session = requests.Session()
create_user_response = session.post(HOST + "/v2/user")

submit_run = session.post(HOST + "/v2/trekkie", json=init_json)
print(submit_run)

run_id = submit_run.json()["trekkie_run"];
session.post(HOST + "/v2/trekkie/" + run_id + "/gpx", files=files)

session.delete(HOST + "/v2/trekkie/" + run_id )

