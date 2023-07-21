import requests
import logging
import http.client as http_client

baseurl = "http://localhost:8060"

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

with requests.Session() as s:
    create_user_response = s.post("{}/user/create".format(baseurl))
    print(create_user_response)
    print(create_user_response.json())
    print(s.cookies.get_dict())

    # try to get session cookie explicitly
    login_response = s.post("{}/user/login".format(baseurl), json=create_user_response.json())
    print(login_response)
    print(login_response.json())
    print(s.cookies.get_dict())

    submit_gpx = s.post("{}/travel/submit/gpx".format(baseurl), files=files, cookies=s.cookies.get_dict())
    print(submit_gpx)
    print(submit_gpx.json())

    times_json["gpx_id"] = submit_gpx.json()["gpx_id"]
    submit_run = s.post("{}/travel/submit/run".format(baseurl), json=times_json, cookies=s.cookies.get_dict())
    print(submit_run)
    print(submit_run.json())

    list_run = s.get("{}/travel/submit/list".format(baseurl), cookies=s.cookies.get_dict())
    print(list_run)
    print(list_run.json())
