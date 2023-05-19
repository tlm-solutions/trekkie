# returns user_id and password
curl -X POST https://trekkie.staging.dvb.solutions/user

# login to get session cookie
curl -X POST -c ./cookie https://trekkie.staging.dvb.solutions/auth/login \
    -H "Content-Type: application/json" \
    -d '{"user_id":"27185202-4b36-4283-9644-f5bf344766e3","password":"vbJh6vZAKiCMJoq5vfJiSUtrkoYRQaNO"}' \ 


JSON_DATA = '    
    {
        "start":"2022-09-10T14:46:30.290072949",
        "stop":"2022-09-10T15:16:25.754147203",
        "line":63,
        "run":8,
        "region": 0
    }
'

# create trekkie run
curl -X POST -c ./cookie https://trekkie.staging.dvb.solutions/trekkie \
    -H "Content-Type: application/json" \
    -b ./cookie \
    -d  $JSON_DATA

# upload gpx file for trekkie run
curl -X POST -b ./cookie https://trekkie.staging.dvb.solutions/trekkie/{trekkie-id}/gpx -F "file=./test.gpx" 

