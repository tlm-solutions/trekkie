import requests
import logging
import http.client as http_client

OFFLINE = False
VERBOSE = False

OFFLINE_HOST = "http://localhost:8060"
STAGING_HOST = "https://trekkie.staging.dvb.solutions"

HOST = OFFLINE_HOST if OFFLINE else STAGING_HOST

files = {"upload_file": open("test.gpx", "rb")}

times_json =  {
            "start": "2022-09-10T14:46:30.290072949",
            "stop": "2022-09-10T15:16:25.754147203",
            "line": 63,
            "run": 8,
            "region": 0,
}

# this enables higly verbose logging for debug purposes
http_client.HTTPConnection.debuglevel = 1
logging.basicConfig()
logging.getLogger().setLevel(logging.DEBUG)
requests_log = logging.getLogger("requests.packages.urllib3")
requests_log.setLevel(logging.DEBUG)
requests_log.propagate = True

session = requests.Session()
create_user_response = session.post(HOST + "/user/create")

submit_run = session.post(HOST + "/travel/submit/run", json=times_json)
print(submit_run)

times_json["trekkie_run"] = submit_run.json()["trekkie_run"]
submit_gpx = session.post(HOST + "/travel/submit/gpx", files=files, json=times_json)


