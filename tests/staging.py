import requests
import logging
import http.client as http_client

baseurl = 'http://localhost:8060'

files = {'upload_file': open('test.gpx','rb')}

times_json = {
    "gpx_id": "fillme",
    "vehicles": [
        {
            "start":"2022-09-10T14:46:30.290072949",
            "stop":"2022-09-10T15:16:25.754147203",
            "line":63,
            "run":8,
            "region": 0
        },
        {
            "start":"2022-09-10T15:22:30.290072949",
            "stop":"2022-09-10T15:29:25.754147203",
            "line":7,
            "run":27,
            "region": 0
        }
    ]
}

# this enables higly verbose logging for debug purposes
#http_client.HTTPConnection.debuglevel = 1
#logging.basicConfig()
#logging.getLogger().setLevel(logging.DEBUG)
#requests_log = logging.getLogger("requests.packages.urllib3")
#requests_log.setLevel(logging.DEBUG)
#requests_log.propagate = True

with requests.Session() as s:
    create_user_response = s.post('{}/user/create'.format(baseurl))
    print(create_user_response)

    submit_gpx = s.post('{}/travel/submit/gpx'.format(baseurl), files = files)
    print(submit_gpx)

    times_json["gpx_id"] = submit_gpx.json()["gpx_id"]
    submit_run = s.post('{}/travel/submit/run'.format(baseurl), json = times_json)
    print(submit_run)

    list_run = s.get('{}/travel/submit/list'.format(baseurl))
    print(list_run)
