curl -X POST https://trekkie.staging.dvb.solutions/user/create
curl -X POST -c ./cookie https://trekkie.staging.dvb.solutions/user/login \
    -H "Content-Type: application/json" \
    -d '{"user_id":"27185202-4b36-4283-9644-f5bf344766e3","password":"vbJh6vZAKiCMJoq5vfJiSUtrkoYRQaNO"}' \ 

GPXID=$(curl -X POST -b ./cookie https://trekkie.staging.dvb.solutions/travel/submit/gpx -F "file=./test.gpx" | jq -r '.gpx_id')

echo $GPXID

JSON_DATA = '{    
    "gpx_id": "$GPXID",
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
]}'

curl -X POST -c ./cookie https://trekkie.staging.dvb.solutions/travel/submit/run \
    -H "Content-Type: application/json" \
    $(jq '.gpx_id = "$GPXID"' <<< "$JSON_DATA") 
    -b ./cookie \
    -d  -vvv

