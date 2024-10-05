contract storage_types {
            
    uint64 public temporary sesa = 1;
    uint64 public instance sesa1 = 1;
    uint64 public persistent sesa2 = 2;
    uint64 public sesa3 = 2;

    function inc() public {
        sesa++;
        sesa1++;
        sesa2++;
        sesa3++;
    }

    function dec() public {
        sesa--;
        sesa1--;
        sesa2--;
        sesa3--;
    }
}